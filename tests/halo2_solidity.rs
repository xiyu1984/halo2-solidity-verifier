use std::marker::PhantomData;

use halo2_proofs::{
    circuit::{AssignedCell, Chip, Layouter, Region, SimpleFloorPlanner, Value}, 
    plonk::{Advice, Circuit, Column, ConstraintSystem, Instance, Selector}, 
    poly::Rotation
};

trait PythagorasInstruction<F: halo2_proofs::arithmetic::Field>: Chip<F> {
    type Num;

    /// Loads a number into the circuit as a private input.
    fn load_private(
        &self,
        layouter: impl Layouter<F>,
        a: Value<F>,
    ) -> Result<Self::Num, halo2_proofs::plonk::Error>;

    /// Returns `c = a * b`.
    fn square(
        &self,
        layouter: impl Layouter<F>,
        a: Self::Num
    ) -> Result<Self::Num, halo2_proofs::plonk::Error>;

    /// Returns `c = a + b`
    fn add(
        &self,
        layouter: impl Layouter<F>,
        a: Self::Num,
        b: Self::Num
    ) -> Result<Self::Num, halo2_proofs::plonk::Error>;

    /// Exposes a number as a public input to the circuit.
    fn expose_public(
        &self,
        layouter: impl Layouter<F>,
        num: Self::Num,
        row: usize,
    ) -> Result<(), halo2_proofs::plonk::Error>;
}

#[derive(Clone, Debug)]
struct PythagorasConfig {
    advice: [Column<Advice>; 2],

    /// This is the public input (instance) column.
    instance: Column<Instance>,

    s_mul: Selector,
    s_add: Selector,
}

struct PythagorasChip<F: halo2_proofs::arithmetic::Field> {
    config: PythagorasConfig,
    _marker: PhantomData<F>,
}

impl<F: halo2_proofs::arithmetic::Field> Chip<F> for PythagorasChip<F> {
    type Config = PythagorasConfig;
    type Loaded = ();

    fn config(&self) -> &Self::Config {
        &self.config
    }

    fn loaded(&self) -> &Self::Loaded {
        &()
    }
}

impl<F: halo2_proofs::arithmetic::Field> PythagorasChip<F> {
    fn construct(config: <Self as Chip<F>>::Config) -> Self {
        Self {
            config,
            _marker: PhantomData,
        }
    }

    fn configure(
        meta: &mut ConstraintSystem<F>,
        advice: [Column<Advice>; 2],
        instance: Column<Instance>
    ) -> <Self as Chip<F>>::Config {
        meta.enable_equality(instance);
        for column in &advice {
            meta.enable_equality(*column);
        }
        let s_mul = meta.selector();
        let s_add = meta.selector();

        // Define our gate!
        // meta.create_gate("mul", |meta| {
        //     let lhs = meta.query_advice(advice[0], Rotation::cur());
        //     let rhs = meta.query_advice(advice[1], Rotation::cur());
        //     let out = meta.query_advice(advice[0], Rotation::next());
        //     let s_mul = meta.query_selector(s_mul);

        //     vec![s_mul * (lhs * rhs - out)]
        // });

        // meta.create_gate("add", |meta| {
        //     let lhs = meta.query_advice(advice[0], Rotation::cur());
        //     let rhs = meta.query_advice(advice[1], Rotation::cur());
        //     let out = meta.query_advice(advice[0], Rotation::next());
        //     let s_add = meta.query_selector(s_add);

        //     vec![s_add * (lhs + rhs - out)]
        // });

        // define out gate (the same as above)
        meta.create_gate("squre_or_add", |meta| {
            let lhs = meta.query_advice(advice[0], Rotation::cur());
            let rhs = meta.query_advice(advice[1], Rotation::cur());
            let out = meta.query_advice(advice[0], Rotation::next());
            let s_mul = meta.query_selector(s_mul);
            let s_add = meta.query_selector(s_add);

            vec![
                s_mul * (lhs.clone() * rhs.clone() - out.clone()),
                s_add * (lhs + rhs - out)
            ]
        });

        PythagorasConfig {
            advice,
            instance,
            s_mul,
            s_add
        }
    }
}

#[derive(Clone)]
struct Number<F: halo2_proofs::arithmetic::Field>(AssignedCell<F, F>);

impl<F: halo2_proofs::arithmetic::Field> PythagorasInstruction<F> for PythagorasChip<F> {
    type Num = Number<F>;

    fn load_private(
        &self,
        mut layouter: impl Layouter<F>,
        a: Value<F>,
    ) -> Result<Self::Num, halo2_proofs::plonk::Error> {
        let config = self.config();

        layouter.assign_region(
            || "load private",
            |mut region| {
                region
                    .assign_advice(|| "private input", config.advice[0], 0, || a)
                    .map(Number)
            },
        )
    }

    fn square(
        &self,
        mut layouter: impl Layouter<F>,
        a: Self::Num
    ) -> Result<Self::Num, halo2_proofs::plonk::Error> {
        let config = self.config();

        layouter.assign_region(
            || "square",
            |mut region: Region<'_, F>| {
                config.s_mul.enable(&mut region, 0)?;

                a.0.copy_advice(|| "lhs", &mut region, config.advice[0], 0)?;
                a.0.copy_advice(|| "rhs", &mut region, config.advice[1], 0)?;

                let value = a.0.value().copied() * a.0.value();

                region
                    .assign_advice(|| "side^2", config.advice[0], 1, || value)
                    .map(Number)
            },
        )
    }

    fn add(
        &self,
        mut layouter: impl Layouter<F>,
        a: Self::Num,
        b: Self::Num
    ) -> Result<Self::Num, halo2_proofs::plonk::Error> {
        let config = self.config();

        layouter.assign_region(|| "add", |mut region: Region<'_, F>| {
            config.s_add.enable(&mut region, 0)?;

            a.0.copy_advice(|| "lhs", &mut region, config.advice[0], 0)?;
            b.0.copy_advice(|| "rhs", &mut region, config.advice[1], 0)?;

            let res = a.0.value().copied() + b.0.value();

            region.assign_advice(|| "hypotenuse^2", config.advice[0], 1, || res).map(Number)
        })
    }

    fn expose_public(
        &self,
        mut layouter: impl Layouter<F>,
        num: Self::Num,
        row: usize,
    ) -> Result<(), halo2_proofs::plonk::Error> {
        let config = self.config();

        layouter.constrain_instance(num.0.cell(), config.instance, row)
    }
}

#[derive(Default, Clone)]
struct P8sTestCircuit<F: halo2_proofs::arithmetic::Field> {
    side_a: Value<F>,
    side_b: Value<F>,
}

impl<F: halo2_proofs::arithmetic::Field> P8sTestCircuit<F> {
    fn new(side_a: Value<F>, side_b: Value<F>) -> Self {
        P8sTestCircuit { side_a, side_b }
    }
}

impl<F: halo2_proofs::arithmetic::Field> Circuit<F> for P8sTestCircuit<F> {
    // Since we are using a single chip for everything, we can just reuse its config.
    type Config = PythagorasConfig;
    type FloorPlanner = SimpleFloorPlanner;
    #[cfg(feature = "circuit-params")]
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let advice = [meta.advice_column(), meta.advice_column()];
        let instance = meta.instance_column();

        PythagorasChip::configure(meta, advice, instance)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), halo2_proofs::plonk::Error> {
        let field_chip = PythagorasChip::<F>::construct(config);

        // Load our private values into the circuit.
        let a = field_chip.load_private(layouter.namespace(|| "load a"), self.side_a)?;
        let b = field_chip.load_private(layouter.namespace(|| "load b"), self.side_b)?;

        let a_square = field_chip.square(layouter.namespace(|| "side_a^2"), a)?;
        let b_square = field_chip.square(layouter.namespace(|| "side_b^2"), b)?;
        let c = field_chip.add(layouter.namespace(|| "hypotenuse^2"), a_square, b_square)?;

        // Expose the result as a public input to the circuit.
        field_chip.expose_public(layouter.namespace(|| "expose hypotenuse"), c, 0)
    }
}

#[test]
fn test_pythagoras_solidity_verifier() {
    use halo2_proofs::poly::kzg::commitment::ParamsKZG;
    use halo2_proofs::halo2curves::bn256::{Bn256, Fr, G1Affine};
    use halo2_proofs::dev::MockProver;
    use halo2_proofs::plonk::{keygen_vk, keygen_pk, ProvingKey, create_proof, verify_proof};
    use halo2_proofs::transcript::TranscriptWriterBuffer;

    use halo2_solidity_verifier::SolidityGenerator;
    use halo2_solidity_verifier::BatchOpenScheme::Bdfg21;
    use halo2_solidity_verifier::Evm;
    use halo2_solidity_verifier::compile_solidity;
    use halo2_solidity_verifier::encode_calldata;
    use halo2_solidity_verifier::Keccak256Transcript;

    use log::LevelFilter;
    use log::info;
    use colored::Colorize;
    use rand::RngCore;

    fn create_proof_checked(
        params: &ParamsKZG<Bn256>,
        pk: &ProvingKey<G1Affine>,
        circuit: impl Circuit<Fr>,
        instances: &[Fr],
        mut rng: impl RngCore,
    ) -> Vec<u8> {
        use halo2_proofs::poly::kzg::{
            multiopen::{ProverSHPLONK, VerifierSHPLONK},
            strategy::SingleStrategy,
        };
    
        let proof = {
            let mut transcript = Keccak256Transcript::new(Vec::new());
            create_proof::<_, ProverSHPLONK<_>, _, _, _, _>(
                params,
                pk,
                &[circuit],
                &[&[instances]],
                &mut rng,
                &mut transcript,
            )
            .unwrap();
            transcript.finalize()
        };
    
        let result = {
            let mut transcript = Keccak256Transcript::new(proof.as_slice());
            verify_proof::<_, VerifierSHPLONK<_>, _, _, SingleStrategy<_>>(
                params,
                pk.get_vk(),
                SingleStrategy::new(params),
                &[&[instances]],
                &mut transcript,
            )
        };
        assert!(result.is_ok());
        proof
    }

    // env logger
    let mut log_builder = env_logger::Builder::from_default_env();
    log_builder.format_timestamp(None);
    log_builder.filter_level(LevelFilter::Info);
    log_builder.try_init().unwrap();

    // start circuit
    let degree = 10;

    let side_a = Fr::from(2);
    let side_b = Fr::from(3);
    let h = side_a.square() + side_b.square();

    // check with mock
    let p8s_circuit = P8sTestCircuit::new(Value::known(side_a), Value::known(side_b));
    let mock_prover = MockProver::run(degree, &p8s_circuit, vec![vec![h]]).unwrap();
    mock_prover.assert_satisfied();
    info!("{}", "Mock prover passes".green().bold());

    // solidity
    let mut rng = rand::thread_rng();
    let param = ParamsKZG::<Bn256>::setup(degree, &mut rng);

    let vk = keygen_vk(&param, &p8s_circuit).unwrap();
    let pk = keygen_pk(&param, vk, &p8s_circuit).unwrap();
    let generator = SolidityGenerator::new(&param, pk.get_vk(), Bdfg21, 1);     // num_instances: the number of public inputs
    let (verifier_solidity, vk_solidity) = generator.render_separately().unwrap();

    // validate
    let mut evm = Evm::default();
    let verifier_creation_code = compile_solidity(&verifier_solidity);
    let verifier_address = evm.create(verifier_creation_code);
    let vk_creation_code = compile_solidity(&vk_solidity);
    let vk_address = evm.create(vk_creation_code);
    // generates SNARK proof and runs EVM verifier
    info!("{}", "Starting finalization phase".blue().bold());
    let now = std_ops::Instant::now();
    let proof = create_proof_checked(&param, &pk, p8s_circuit.clone(), &vec![h], &mut rng);
    info!("{}", "SNARK proof generated successfully!".green().bold());
    std_ops::report_elapsed(now);
    let calldata = encode_calldata(Some(vk_address.into()), &proof, &vec![h]);
    let (gas_cost, _output) = evm.call(verifier_address, calldata);
    info!("{}", format!("Gas cost: {}", gas_cost).yellow().bold());

    // save verifier and vk as solidity smart contract
    std_ops::save_solidity(format!("p8s_verifier.sol"), &verifier_solidity);
    std_ops::save_solidity(format!("p8s_vk.sol"), &vk_solidity);

}

mod std_ops {
    pub(crate) use std::{
        fs::{create_dir_all, File},
        io::Write
    };
    pub(crate) use std::time::Instant;

    pub(crate) fn save_solidity(name: impl AsRef<str>, solidity: &str) {
        const DIR_GENERATED: &str = "./generated-sc";
    
        create_dir_all(DIR_GENERATED).unwrap();
        File::create(format!("{DIR_GENERATED}/{}", name.as_ref()))
            .unwrap()
            .write_all(solidity.as_bytes())
            .unwrap();
    }

    pub(crate) fn report_elapsed(now: Instant) {
        use colored::Colorize;

        println!(
            "{}",
            format!("Took {} milliseconds", now.elapsed().as_millis())
                .blue()
                .bold()
        );
    }
}
