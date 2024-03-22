use std::marker::PhantomData;

use halo2_proofs::{
    circuit::{Layouter, Value, Chip},
    plonk::{Column, Advice, Selector, Instance, ConstraintSystem, Fixed},
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
    fn mul(
        &self,
        layouter: impl Layouter<F>,
        a: Self::Num,
        b: Self::Num,
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
        instance: Column<Instance>,
        constant: Column<Fixed>,
    ) -> <Self as Chip<F>>::Config {
        meta.enable_equality(instance);
        meta.enable_constant(constant);
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
        meta.create_gate("mul_add", |meta| {
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

// impl<F: halo2_proofs::arithmetic::Field> PythagorasInstruction<F> for PythagorasChip<F> {

// }
