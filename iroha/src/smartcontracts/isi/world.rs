//! This module contains `World` related ISI implementations.

use super::prelude::*;
use crate::prelude::*;

/// Iroha Special Instructions that have `World` as their target.
pub mod isi {
    use iroha_data_model::prelude::*;
    use iroha_error::{error, Result};

    use super::*;

    impl<W: WorldTrait> Execute<W> for Register<Peer> {
        type Error = Error;

        fn execute(
            self,
            _authority: <Account as Identifiable>::Id,
            wsv: &WorldStateView<W>,
        ) -> Result<(), Error> {
            if wsv.trusted_peers_ids().insert(self.object.id) {
                Ok(())
            } else {
                Err(error!("Peer already presented in the list of trusted peers.",).into())
            }
        }
    }

    impl<W: WorldTrait> Execute<W> for Register<Domain> {
        type Error = Error;

        fn execute(
            self,
            _authority: <Account as Identifiable>::Id,
            wsv: &WorldStateView<W>,
        ) -> Result<(), Error> {
            let domain = self.object;
            domain.validate_len(wsv.config.length_limits)?;
            drop(wsv.domains().insert(domain.name.clone(), domain));
            Ok(())
        }
    }

    impl<W: WorldTrait> Execute<W> for Unregister<Domain> {
        type Error = Error;

        fn execute(
            self,
            _authority: <Account as Identifiable>::Id,
            wsv: &WorldStateView<W>,
        ) -> Result<(), Error> {
            // TODO: Should we fail if no domain found?
            drop(wsv.domains().remove(&self.object_id));
            Ok(())
        }
    }

    #[cfg(feature = "roles")]
    impl<W: WorldTrait> Execute<W> for Register<Role> {
        type Error = Error;

        fn execute(
            self,
            _authority: <Account as Identifiable>::Id,
            wsv: &WorldStateView<W>,
        ) -> Result<(), Error> {
            let role = self.object;
            drop(wsv.world.roles.insert(role.id.clone(), role));
            Ok(())
        }
    }

    #[cfg(feature = "roles")]
    impl<W: WorldTrait> Execute<W> for Unregister<Role> {
        type Error = Error;

        fn execute(
            self,
            _authority: <Account as Identifiable>::Id,
            wsv: &WorldStateView<W>,
        ) -> Result<(), Error> {
            drop(wsv.world.roles.remove(&self.object_id));
            for mut domain in wsv.domains().iter_mut() {
                for account in domain.accounts.values_mut() {
                    let _ = account.roles.remove(&self.object_id);
                }
            }
            Ok(())
        }
    }
}

/// Query module provides `IrohaQuery` Peer related implementations.
pub mod query {
    use iroha_data_model::prelude::*;
    use iroha_error::Result;
    use iroha_logger::log;

    use super::*;

    #[cfg(feature = "roles")]
    impl<W: WorldTrait> Query<W> for FindAllRoles {
        #[log]
        fn execute(&self, wsv: &WorldStateView<W>) -> Result<Self::Output> {
            Ok(wsv
                .world
                .roles
                .iter()
                .map(|role| role.value().clone())
                .collect())
        }
    }

    impl<W: WorldTrait> Query<W> for FindAllPeers {
        #[log]
        fn execute(&self, wsv: &WorldStateView<W>) -> Result<Self::Output> {
            Ok(wsv.peers())
        }
    }
}